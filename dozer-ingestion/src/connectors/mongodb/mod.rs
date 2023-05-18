mod schema_inference;
use std::sync::Arc;

use bson::{doc, Document};
use dozer_types::{
    ingestion_types::IngestionMessage,
    log,
    models::connection::MongoConfig as Config,
    ordered_float::OrderedFloat,
    serde_json,
    types::{Field, FieldDefinition, Operation, Record, Schema, SchemaIdentifier},
};
use tonic::async_trait;

use crate::{connectors::SourceSchema, errors::ConnectorError, ingestion::Ingestor};

use super::{Connector, SourceSchemaResult, TableIdentifier, TableInfo};

use mongodb::{
    change_stream::event::OperationType,
    options::{ChangeStreamOptions, ClientOptions, ServerAddress},
    Client,
};

#[derive(Clone, Debug)]
pub struct MongoConfig {
    pub name: String,
    pub config: Config,
}
#[derive(Debug)]
pub struct MongoConnector {
    name: String,
    client: Client,
}

impl MongoConnector {
    pub fn new(config: MongoConfig) -> Self {
        let options: ClientOptions = ClientOptions::builder()
            .app_name(Some(config.name.clone()))
            .hosts(vec![ServerAddress::Tcp {
                host: config.config.host,
                port: Some(config.config.port as u16),
            }])
            .default_database(Some(config.config.database))
            .build();

        let client = Client::with_options(options).unwrap();

        // TODO!
        Self {
            name: config.name,
            client,
        }
    }
}

#[async_trait]
impl Connector for MongoConnector {
    fn types_mapping() -> Vec<(String, Option<dozer_types::types::FieldType>)>
    where
        Self: Sized,
    {
        todo!()
    }

    async fn validate_connection(&self) -> Result<(), ConnectorError> {
        Ok(())
    }

    async fn list_tables(&self) -> Result<Vec<TableIdentifier>, ConnectorError> {
        let mut cursor = self
            .client
            .default_database()
            .unwrap()
            .list_collections(None, None)
            .await
            .unwrap();
        let mut tbls = vec![];
        while cursor.advance().await.unwrap() {
            let collection_spec = cursor.deserialize_current().unwrap();
            let name = collection_spec.name;
            let tbl_iden = TableIdentifier { schema: None, name };
            tbls.push(tbl_iden);
        }

        Ok(tbls)
    }

    async fn validate_tables(&self, tables: &[TableIdentifier]) -> Result<(), ConnectorError> {
        Ok(())
    }

    async fn list_columns(
        &self,
        tables: Vec<TableIdentifier>,
    ) -> Result<Vec<TableInfo>, ConnectorError> {
        println!("tables = {:?}", tables);
        Ok(vec![])
    }

    async fn get_schemas(
        &self,
        table_infos: &[TableInfo],
    ) -> Result<Vec<SourceSchemaResult>, ConnectorError> {
        let mut schemas: Vec<SourceSchemaResult> = vec![];
        for tbl in table_infos {
            println!("tbl.name = {:?}", tbl.name);
            let mut cursor = self
                .client
                .default_database()
                .unwrap()
                .collection::<Document>(&tbl.name)
                .aggregate(vec![doc! {"$sample": {"size": 1000}}], None)
                .await
                .unwrap();
            if let Ok(true) = cursor.advance().await {
                // todo! right now it just takes the first batch & uses that.
                let curr = cursor.deserialize_current().unwrap();
                let flds: Vec<FieldDefinition> = curr
                    .into_iter()
                    .map(|(key, value)| {
                        let dtype = schema_inference::value_to_dtype(&value);
                        FieldDefinition {
                            name: key,
                            nullable: true,
                            typ: dtype,
                            source: dozer_types::types::SourceDefinition::Dynamic,
                        }
                    })
                    .collect();

                let schema = Schema {
                    fields: flds,
                    identifier: Some(SchemaIdentifier { id: 0, version: 1 }),
                    primary_index: vec![],
                };

                schemas.push(Ok(SourceSchema {
                    schema,
                    cdc_type: crate::connectors::CdcType::Nothing,
                }));
            }
        }
        Ok(schemas)
    }

    async fn start(
        &self,
        ingestor: &Ingestor,
        tables: Vec<TableInfo>,
    ) -> Result<(), ConnectorError> {
        let mut count = 0;
        let mut db = Arc::new(self.client.default_database().unwrap());
        for tbl in tables {
            let db = db.clone();
            let mut resume_token = Some(
                serde_json::from_str(
                    r#" {
                "_data": "82645D05F8000000012B0229296E04"
              }"#,
                )
                .unwrap(),
            );
            // tokio::spawn(async move {
            let mut change_stream = db
                .collection::<Document>(&tbl.name)
                .watch(
                    None,
                    Some(
                        ChangeStreamOptions::builder()
                            .resume_after(resume_token)
                            .build(),
                    ),
                )
                .await
                .unwrap();

            while change_stream.is_alive() {
                if let Some(event) = change_stream.next_if_any().await.unwrap() {
                    let record = event.full_document.unwrap();
                    let flds = doc_to_flds(record);
                    let record = Record {
                        schema_id: None,
                        values: flds,
                        lifetime: None,
                    };

                    let op = match event.operation_type {
                        OperationType::Insert => Operation::Insert { new: record },
                        OperationType::Update => {
                            let record_old = event.full_document_before_change.unwrap();
                            let flds = doc_to_flds(record_old);
                            let record_old = Record {
                                schema_id: None,
                                values: flds,
                                lifetime: None,
                            };

                            Operation::Update {
                                old: record_old,
                                new: record,
                            }
                        }
                        OperationType::Replace => {
                            log::info!("OperationType::Replace");
                            todo!("replace")
                        }
                        OperationType::Delete => Operation::Delete { old: record },
                        _ => todo!("other"),
                    };
                    ingestor
                        .handle_message(IngestionMessage::new_op(0, count, op))
                        .unwrap();
                    // if let Some(doc) = event.full_document {
                    //     log::info!("doc = {:?}", doc);
                    // }
                    // process event
                }
                resume_token = change_stream.resume_token();
                let json_token = serde_json::to_string_pretty(&resume_token).unwrap();
            }
        }
        loop {
            let record = Record {
                schema_id: None,

                values: vec![],

                lifetime: None,
            };

            let op = Operation::Insert { new: record };
            // ingestor.handle_message
            // // ingestor
            //     .handle_message(IngestionMessage::new_op(0, count, op.clone()))
            //     .map_err(|e| ConnectorError::InternalError(Box::new(e)))?;

            // count += 1;
        }
    }
}

fn doc_to_flds(doc: Document) -> Vec<Field> {
    use bson::Bson::*;
    doc.values()
        .map(|value| match value {
            Double(v) => Field::Float(OrderedFloat(*v)),
            String(v) => Field::String(v.clone()),
            Boolean(v) => Field::Boolean(*v),
            Null => Field::Null,
            JavaScriptCode(v) => Field::String(v.clone()),
            Int32(v) => Field::Int(*v as i64),
            Int64(v) => Field::Int(*v),
            Binary(v) => Field::Binary(v.bytes.clone()),
            Array(v) => todo!(),
            Document(doc) => todo!("documend"),
            Timestamp(_) => todo!("timestamp"),
            ObjectId(id)  => Field::String(id.to_hex()),
            ty => todo!("ty = {:?}", ty),
        })
        .collect()
}

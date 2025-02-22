// Copyright 2022 RisingLight Project Authors. Licensed under Apache-2.0.

use super::*;
use crate::catalog::{ColumnCatalog, ColumnDesc};
use crate::parser::{ColumnDef, ColumnOption, Statement};
use crate::types::{DataType, DatabaseId, SchemaId};

/// A bound `create table` statement.
#[derive(Debug, PartialEq, Clone)]
pub struct BoundCreateTable {
    pub database_id: DatabaseId,
    pub schema_id: SchemaId,
    pub table_name: String,
    pub columns: Vec<ColumnCatalog>,
}

impl Binder {
    pub fn bind_create_table(&mut self, stmt: &Statement) -> Result<BoundCreateTable, BindError> {
        match stmt {
            Statement::CreateTable { name, columns, .. } => {
                let name = &lower_case_name(name);
                let (database_name, schema_name, table_name) = split_name(name)?;
                let db = self
                    .catalog
                    .get_database_by_name(database_name)
                    .ok_or_else(|| BindError::InvalidDatabase(database_name.into()))?;
                let schema = db
                    .get_schema_by_name(schema_name)
                    .ok_or_else(|| BindError::InvalidSchema(schema_name.into()))?;
                if schema.get_table_by_name(table_name).is_some() {
                    return Err(BindError::DuplicatedTable(table_name.into()));
                }
                // check duplicated column names
                let mut set = HashSet::new();
                for col in columns.iter() {
                    if !set.insert(col.name.value.to_lowercase()) {
                        return Err(BindError::DuplicatedColumn(col.name.value.clone()));
                    }
                }
                let columns = columns
                    .iter()
                    .enumerate()
                    .map(|(idx, col)| {
                        let mut col = ColumnCatalog::from(col);
                        col.set_id(idx as ColumnId);
                        col
                    })
                    .collect();
                Ok(BoundCreateTable {
                    database_id: db.id(),
                    schema_id: schema.id(),
                    table_name: table_name.into(),
                    columns,
                })
            }
            _ => panic!("mismatched statement type"),
        }
    }
}

impl From<&ColumnDef> for ColumnCatalog {
    fn from(cdef: &ColumnDef) -> Self {
        let mut is_nullable = true;
        let mut is_primary_ = false;
        for opt in &cdef.options {
            match opt.option {
                ColumnOption::Null => is_nullable = true,
                ColumnOption::NotNull => is_nullable = false,
                ColumnOption::Unique { is_primary } => is_primary_ = is_primary,
                _ => todo!("column options"),
            }
        }
        ColumnCatalog::new(
            0,
            ColumnDesc::new(
                DataType::new(cdef.data_type.clone(), is_nullable),
                cdef.name.value.to_lowercase(),
                is_primary_,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::catalog::RootCatalog;
    use crate::parser::parse;
    use crate::types::{DataTypeExt, DataTypeKind};

    #[test]
    fn bind_create_table() {
        let catalog = Arc::new(RootCatalog::new());
        let mut binder = Binder::new(catalog.clone());
        let sql = "
            create table t1 (v1 int not null, v2 int); 
            create table t2 (a int not null, a int not null);
            create table t3 (v1 int not null);";
        let stmts = parse(sql).unwrap();

        assert_eq!(
            binder.bind_create_table(&stmts[0]).unwrap(),
            BoundCreateTable {
                database_id: 0,
                schema_id: 0,
                table_name: "t1".into(),
                columns: vec![
                    ColumnCatalog::new(
                        0,
                        DataTypeKind::Int(None).not_null().to_column("v1".into())
                    ),
                    ColumnCatalog::new(
                        1,
                        DataTypeKind::Int(None).nullable().to_column("v2".into())
                    ),
                ],
            }
        );

        assert_eq!(
            binder.bind_create_table(&stmts[1]),
            Err(BindError::DuplicatedColumn("a".into()))
        );

        let database = catalog.get_database_by_id(0).unwrap();
        let schema = database.get_schema_by_id(0).unwrap();
        schema.add_table("t3".into(), vec![], false).unwrap();
        assert_eq!(
            binder.bind_create_table(&stmts[2]),
            Err(BindError::DuplicatedTable("t3".into()))
        );
    }
}

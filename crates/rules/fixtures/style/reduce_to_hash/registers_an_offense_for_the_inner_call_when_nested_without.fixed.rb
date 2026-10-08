tables.to_h { |table| [table.node, table.columns.to_h { |column| [column.name, column.alias] }] }

Model.where(other_table: { column_name: OtherModel.select(:column_name) }).map(&:column_name)

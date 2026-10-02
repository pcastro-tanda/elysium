Model.select { |item| item.column_name }.map(&:column_name)

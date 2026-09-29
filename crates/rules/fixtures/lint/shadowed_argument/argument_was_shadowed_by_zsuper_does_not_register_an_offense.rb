def select_fields(query, current_time)
  query = super
  query.select('*')
end

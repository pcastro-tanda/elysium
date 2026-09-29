def select_fields(query, current_time)
  query = super
  ^^^^^^^^^^^^^ Argument `query` was shadowed by a local variable before it was used.
  query.select('*')
end

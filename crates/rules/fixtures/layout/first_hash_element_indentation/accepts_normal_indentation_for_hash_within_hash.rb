scope = scope.where(
  klass.table_name => {
    reflection.type => model.base_class.sti_name
  }
)

has_many opt: lambda {
  where(cats: Cat.young.where_values_hash)
}

has_many(
  opt: lambda do
    where(cats: Cat.young.where_values_hash)
  end
)

has_many :kittens, lambda {
  where(cats: Cat.young.where_values_hash)
}, source: cats

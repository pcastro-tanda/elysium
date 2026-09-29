has_many :kittens, -> do
                   ^^ Use the `lambda` method for multiline lambdas.
  where(cats: Cat.young.where_values_hash)
end, source: cats

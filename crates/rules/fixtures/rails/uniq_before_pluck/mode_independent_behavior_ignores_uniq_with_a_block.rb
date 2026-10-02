Model.where(foo: 1).pluck(:name).uniq { |k| k[0] }

Model.where(foo: 1).pluck(:name).uniq { _1[0] }

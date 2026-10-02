Model.where(foo: 1).pluck(:name).uniq { it[0] }

Foo.where(id: Bar.select(:id)
                 .joins(:bar))

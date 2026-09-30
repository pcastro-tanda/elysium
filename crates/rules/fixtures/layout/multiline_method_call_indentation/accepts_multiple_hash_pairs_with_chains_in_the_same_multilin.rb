Foo
  .where(id: Bar.select(:id)
    .joins(:bar),
         name: Car.find(:name)
    .strip)

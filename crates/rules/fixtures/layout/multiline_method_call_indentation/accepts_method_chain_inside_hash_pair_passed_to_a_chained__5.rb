@foo = Foo
  .where(id: Bar.select(:id)
    .joins(:bar)
    .where.not(bar: { id: 123 }))

item.with_lock do
  return if item.stock == 0
  item.update!(foobar: true)
end

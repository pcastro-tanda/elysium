items.each(lambda do |item|
  return if item.stock == 0
  item.update!(foobar: true)
end)
items.each -> (item) {
  return if item.stock == 0
  item.update!(foobar: true)
}

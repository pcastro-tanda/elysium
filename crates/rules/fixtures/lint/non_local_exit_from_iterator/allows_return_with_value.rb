def find_first_sold_out_item(items)
  items.each do |item|
    return item if item.stock == 0
    item.foobar!
  end
end

def some_method(&block)
  items.each do |item|
    yield item
  end
end

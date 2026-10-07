sig { params(items: T::Array[Integer]).void }
def initialize(items)
  items.each do |item|
    @item = T.let(item, Integer)
  end
end

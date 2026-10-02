def test(a)
  define_singleton_method(:test2) do |a|
    super(a)
  end
  b.define_singleton_method(:test2) do |a|
    super(a)
  end
end

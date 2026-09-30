self::A = Class.new do
  def foo
    1
  end
  def foo
  ^^^^^^^ Method `::A#foo` is defined at both (string):2 and (string):5.
    2
  end
end

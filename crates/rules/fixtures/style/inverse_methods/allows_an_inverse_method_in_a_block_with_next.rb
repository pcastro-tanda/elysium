class TestClass
  def test_method
    [1, 2, 3, 4].select do |number|
      next if number == 4

      number != 2
    end
  end
end

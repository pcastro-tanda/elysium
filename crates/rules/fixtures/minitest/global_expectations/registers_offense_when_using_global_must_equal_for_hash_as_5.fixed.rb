it 'does something' do
  @n = do_something
  expect(@n[:foo]).must_equal 42
end

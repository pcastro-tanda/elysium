it 'does something' do
  @n = do_something
  expect(@n[:foo]).must_be 42
end

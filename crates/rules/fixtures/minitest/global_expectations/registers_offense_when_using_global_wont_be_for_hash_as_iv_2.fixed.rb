it 'does something' do
  @n = do_something
  expect(@n[:foo]).wont_be 42
end

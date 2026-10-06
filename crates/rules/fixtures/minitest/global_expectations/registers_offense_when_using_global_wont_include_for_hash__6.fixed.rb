it 'does something' do
  @n = do_something
  expect(@n[:foo]).wont_include 42
end

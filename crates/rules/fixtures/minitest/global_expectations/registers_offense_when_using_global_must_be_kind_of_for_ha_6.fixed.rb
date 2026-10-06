it 'does something' do
  @n = do_something
  expect(@n[:foo]).must_be_kind_of 42
end

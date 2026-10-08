it 'does something' do
  @n = do_something
  value(@n).wont_be 42
end

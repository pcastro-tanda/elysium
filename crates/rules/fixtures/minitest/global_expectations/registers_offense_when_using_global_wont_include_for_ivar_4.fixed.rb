it 'does something' do
  @n = do_something
  value(@n).wont_include 42
end

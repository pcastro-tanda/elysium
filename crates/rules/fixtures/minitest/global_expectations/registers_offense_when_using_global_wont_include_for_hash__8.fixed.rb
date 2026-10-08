it 'does something' do
  @n = do_something
  value(@n[:foo]).wont_include 42
end

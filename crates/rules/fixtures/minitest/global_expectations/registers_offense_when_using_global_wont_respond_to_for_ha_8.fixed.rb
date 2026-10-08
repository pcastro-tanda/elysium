it 'does something' do
  @n = do_something
  value(@n[:foo]).wont_respond_to 42
end

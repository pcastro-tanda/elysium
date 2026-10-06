it 'does something' do
  @n = do_something
  @n.wont_respond_to 42
  ^^ Use `value(@n)` instead.
end

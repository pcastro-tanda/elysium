it 'does something' do
  @n = do_something
  @n.wont_be_within_epsilon 42
  ^^ Use `value(@n)` instead.
end

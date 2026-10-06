it 'does something' do
  @n = do_something
  @n.wont_be 42
  ^^ Use `_(@n)` instead.
end

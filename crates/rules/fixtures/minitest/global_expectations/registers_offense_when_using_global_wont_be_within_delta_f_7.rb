it 'does something' do
  @n = do_something
  @n.wont_be_within_delta 42
  ^^ Use `_(@n)` instead.
end

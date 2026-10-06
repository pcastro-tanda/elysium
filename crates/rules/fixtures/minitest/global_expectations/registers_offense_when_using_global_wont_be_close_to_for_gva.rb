it 'does something' do
  $n = do_something
  $n.wont_be_close_to 42
  ^^ Use `_($n)` instead.
end

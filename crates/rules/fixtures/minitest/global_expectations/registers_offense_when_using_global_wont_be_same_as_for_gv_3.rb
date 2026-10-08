it 'does something' do
  $n = do_something
  $n.wont_be_same_as 42
  ^^ Use `_($n)` instead.
end

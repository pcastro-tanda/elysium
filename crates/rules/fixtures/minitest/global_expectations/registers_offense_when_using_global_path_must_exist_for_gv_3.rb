it 'does something' do
  $n = do_something
  $n.path_must_exist 42
  ^^ Use `_($n)` instead.
end

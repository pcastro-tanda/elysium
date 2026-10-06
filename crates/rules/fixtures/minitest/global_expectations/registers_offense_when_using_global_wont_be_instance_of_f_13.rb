it 'does something' do
  $n = do_something
  $n.wont_be_instance_of 42
  ^^ Use `_($n)` instead.
end

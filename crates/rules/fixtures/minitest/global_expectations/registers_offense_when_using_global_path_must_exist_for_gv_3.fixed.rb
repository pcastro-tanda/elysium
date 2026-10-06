it 'does something' do
  $n = do_something
  value($n).path_must_exist 42
end

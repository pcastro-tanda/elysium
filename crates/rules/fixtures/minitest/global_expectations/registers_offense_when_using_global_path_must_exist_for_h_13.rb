it 'does something' do
  $n = do_something
  $n[:foo].path_must_exist 42
  ^^^^^^^^ Use `_($n[:foo])` instead.
end

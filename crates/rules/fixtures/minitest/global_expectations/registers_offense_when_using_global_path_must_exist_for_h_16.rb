it 'does something' do
  $n = do_something
  $n[:foo].path_must_exist 42
  ^^^^^^^^ Use `value($n[:foo])` instead.
end

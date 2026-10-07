it 'does something' do
  $n = do_something
  $n[:foo].must_be_instance_of 42
  ^^^^^^^^ Use `value($n[:foo])` instead.
end

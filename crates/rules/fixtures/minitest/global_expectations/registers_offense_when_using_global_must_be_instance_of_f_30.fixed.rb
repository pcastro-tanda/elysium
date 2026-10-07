it 'does something' do
  $n = do_something
  expect($n[:foo]).must_be_instance_of 42
end

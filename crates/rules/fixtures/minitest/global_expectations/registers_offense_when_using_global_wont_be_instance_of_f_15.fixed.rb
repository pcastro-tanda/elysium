it 'does something' do
  $n = do_something
  _($n).wont_be_instance_of 42
end

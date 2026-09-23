hash = {
  'abcdefg' => 0,
  'abcdef'  => 0,
  ^^^^^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
  'gijk' => 0
}

hash = {
  'abcdefg' => 0,
  'abcdef'       => 0,
  ^^^^^^^^^^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
  'gijk' => 0
}

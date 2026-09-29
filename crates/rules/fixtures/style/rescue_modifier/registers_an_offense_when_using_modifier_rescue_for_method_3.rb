method(<<~EOS1, <<~EOS2) rescue handle
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `rescue` in its modifier form.
  str1
EOS1
  str2
EOS2

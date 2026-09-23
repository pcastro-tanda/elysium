build(:house,
  :rooms => [
  ^^^^^^^^^^^ Align the arguments of a method call if they span more than one line.
    build(:bedroom,
      :bed => build(:bed,
      ^^^^^^^^^^^^^^^^^^^ Align the arguments of a method call if they span more than one line.
        :occupants => [],
        ^^^^^^^^^^^^^^^^^ Align the arguments of a method call if they span more than one line.
        :size => "king"
      )
    )
  ]
)

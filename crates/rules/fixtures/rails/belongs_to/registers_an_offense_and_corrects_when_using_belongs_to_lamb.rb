belongs_to :foo, -> { bar }, required: false
^^^^^^^^^^ You specified `required: false`, in Rails > 5.0 the required option is deprecated and you want to use `optional: true`.

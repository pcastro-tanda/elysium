belongs_to :foo, required: true
^^^^^^^^^^ You specified `required: true`, in Rails > 5.0 the required option is deprecated and you want to use `optional: false`. In most configurations, this is the default and you can omit this option altogether

ospf_h = {
  'ospfTest'    => {
    'foo'      => {
      area: '0.0.0.0', cost: 10, hello: 30, pass: true },
    'longname' => {
      area: '1.1.1.38', pass: false },
    'vlan101'  => {
      area: '2.2.2.101', cost: 5, hello: 20, pass: true }
  },
  'TestOspfInt' => {
    'x'               => {
      area: '0.0.0.19' },
    'vlan290'         => {
      area: '2.2.2.29', cost: 200, hello: 30, pass: true },
    'port-channel100' => {
      area: '3.2.2.29', cost: 25, hello: 50, pass: false }
  }
}

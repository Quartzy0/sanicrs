datadir = get_option('prefix') / get_option('datadir')

application_id = 'me.quartzy.sanicrs'
# Read more <https://mesonbuild.com/Reference-manual.html#install_data>

install_data(
    '@0@.desktop'.format(application_id),
    install_dir: datadir / 'applications'
)

install_data(
    '@0@.metainfo.xml'.format(application_id),
    install_dir: datadir / 'metainfo'
)

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
 test('Validate appstream file',
   appstreamcli,
   args: ['validate', '--no-net', '--explain', '@0@.metainfo.xml'.format(application_id)],
   suite: ['lint'],
 )
endif


install_data('@0@.gschema.xml'.format(application_id),
  install_dir: get_option('datadir') / 'glib-2.0/schemas',
)

gnome.compile_schemas()

compile_schemas = find_program('glib-compile-schemas', required: false)
if compile_schemas.found()
  test('Validate schema file',
    compile_schemas,
    args: ['--strict', '--dry-run', meson.current_source_dir()],
    suite: ['lint'],
  )
endif

service_conf = configuration_data()
service_conf.set('application_id', application_id)
service_conf.set('bindir', get_option('prefix') / get_option('bindir'))
configure_file(
  input: 'me.quartzy.sanicrs.service.in',
  output: '@0@.service'.format(application_id),
  configuration: service_conf,
  install: true,
  install_dir: get_option('datadir') / 'dbus-1/services',
)

subdir('icons')


#install_data(
#    '@0@.svg'.format(application_id),
#    install_dir: datadir / 'icons' / 'hicolor' / 'scalable' / 'apps'
#)

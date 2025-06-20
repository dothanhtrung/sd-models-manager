create table base
(
    id    integer not null
        constraint base_pk_2
            primary key autoincrement,
    label TEXT
        constraint base_pk
            unique
);

create table model
(
    id      integer not null
        constraint model_pk
            primary key autoincrement,
    path    integer not null
        constraint model_pk_2
            unique,
    base_id integer not null
        constraint model_root_id_fk
            references base
            on delete cascade
);

create table tag
(
    id          integer not null
        constraint tag_pk
            primary key autoincrement,
    name        TEXT    not null
        constraint tag_pk_2
            unique,
    description integer
);

create unique index tag_name_uindex
    on tag (name);

create table tag_model
(
    tag   integer
        constraint tag_model_tag_id_fk
            references tag
            on delete cascade,
    model integer
        constraint tag_model_model_id_fk
            references model
            on delete cascade
);

create unique index tag_model_tag_model_uindex
    on tag_model (tag, model);

